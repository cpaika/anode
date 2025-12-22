pub mod bootstrap;
pub mod discovery;
pub mod membership;
pub mod rebalance;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub use bootstrap::{BootstrapConfig, BootstrapError, ClusterBootstrap};
pub use discovery::{Discovery, DiscoveryError, DiscoveryMethod, NodeInfo, StaticDiscovery};
pub use membership::{MembershipError, MembershipManager, NodeHealth};
pub use rebalance::{
    ChunkTransfer, RebalanceConfig, RebalanceError, RebalancePlan, RebalanceResult,
    RebalanceStatus, Rebalancer,
};

// Re-export NodeId from anode_raft
pub use anode_raft::NodeId;

/// Cluster status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub cluster_name: String,
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub leader_id: Option<NodeId>,
    pub current_term: u64,
    pub nodes: Vec<NodeInfo>,
}

/// Cluster manager for coordinating cluster operations
pub struct ClusterManager {
    raft: Arc<anode_raft::AnodeRaft>,
    membership_manager: Arc<RwLock<MembershipManager>>,
    placement_groups: u32,
}

impl ClusterManager {
    /// Create a new cluster manager
    pub async fn new(raft: Arc<anode_raft::AnodeRaft>, placement_groups: u32) -> Self {
        let discovery = Arc::new(Discovery::new());
        let membership_manager =
            Arc::new(RwLock::new(MembershipManager::new(raft.clone(), discovery)));
        Self {
            raft,
            membership_manager,
            placement_groups,
        }
    }

    /// Get the current cluster status
    pub async fn get_status(&self) -> ClusterStatus {
        let metrics = self.raft.metrics().borrow().clone();
        let membership = self.membership_manager.read().await;

        // Get node information from membership
        let nodes = vec![]; // TODO: Implement node list from membership

        ClusterStatus {
            cluster_name: "anode-cluster".to_string(), // TODO: Get from config
            total_nodes: metrics.membership_config.membership().voter_ids().count(),
            healthy_nodes: metrics.membership_config.membership().voter_ids().count(), // TODO: Implement health check
            leader_id: metrics.current_leader,
            current_term: metrics.current_term,
            nodes,
        }
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, node_info: NodeInfo) -> Result<(), MembershipError> {
        let membership = self.membership_manager.read().await;
        membership.add_node(node_info).await
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: NodeId) -> Result<(), MembershipError> {
        let membership = self.membership_manager.read().await;
        membership.remove_node(node_id).await
    }
}
