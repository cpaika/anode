use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use anode_raft::AnodeRaft;

// Re-export cluster submodules
pub mod bootstrap;
pub mod discovery;
pub mod membership;
pub mod rebalance;

pub use bootstrap::{BootstrapConfig, BootstrapError, ClusterBootstrap};
pub use discovery::{Discovery, DiscoveryError, DiscoveryMethod, NodeInfo};
pub use membership::{MembershipError, MembershipManager, NodeHealth, HealthStatus};
pub use rebalance::{ChunkTransfer, RebalanceConfig, RebalanceError, RebalancePlan, RebalanceResult, RebalanceStatus, Rebalancer};

pub type NodeId = u64;

#[derive(Error, Debug)]
pub enum ClusterError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("Node already exists: {0}")]
    NodeAlreadyExists(NodeId),

    #[error("Cluster not initialized")]
    NotInitialized,

    #[error("Not cluster leader")]
    NotLeader,

    #[error("Raft error: {0}")]
    RaftError(String),

    #[error("Invalid placement group: {0}")]
    InvalidPlacementGroup(u32),
}

/// Placement group for data distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementGroup {
    pub id: u32,
    pub nodes: Vec<NodeId>,
}

/// Overall cluster status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub leader_id: Option<NodeId>,
    pub members: Vec<NodeInfo>,
    pub placement_groups: Vec<PlacementGroup>,
    pub total_nodes: usize,
    pub healthy_nodes: usize,
}

/// Manages cluster membership and placement groups
pub struct ClusterManager {
    raft: Arc<AnodeRaft>,
    members: RwLock<HashMap<NodeId, NodeInfo>>,
    placement_groups: RwLock<Vec<PlacementGroup>>,
    num_placement_groups: u32,
}

impl ClusterManager {
    /// Create a new cluster manager
    pub async fn new(raft: Arc<AnodeRaft>, num_placement_groups: u32) -> Self {
        let placement_groups = (0..num_placement_groups)
            .map(|id| PlacementGroup {
                id,
                nodes: Vec::new(),
            })
            .collect();

        Self {
            raft,
            members: RwLock::new(HashMap::new()),
            placement_groups: RwLock::new(placement_groups),
            num_placement_groups,
        }
    }

    /// Get the current cluster status
    pub async fn get_status(&self) -> ClusterStatus {
        let members = self.members.read().await;
        let placement_groups = self.placement_groups.read().await;

        let leader_id = self.get_leader().await;

        ClusterStatus {
            leader_id,
            members: members.values().cloned().collect(),
            placement_groups: placement_groups.clone(),
            total_nodes: members.len(),
            healthy_nodes: members.len(), // TODO: Track health status
        }
    }

    /// Get the current cluster leader
    pub async fn get_leader(&self) -> Option<NodeId> {
        match self.raft.current_leader().await {
            Some(leader) => Some(leader),
            None => None,
        }
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, node: NodeInfo) -> Result<(), ClusterError> {
        info!("Adding node {} ({}) to cluster", node.id, node.name);

        let mut members = self.members.write().await;

        if members.contains_key(&node.id) {
            return Err(ClusterError::NodeAlreadyExists(node.id));
        }

        members.insert(node.id, node.clone());
        drop(members);

        // Rebalance placement groups to include the new node
        self.rebalance().await?;

        info!("Node {} added successfully", node.id);
        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: NodeId) -> Result<(), ClusterError> {
        info!("Removing node {} from cluster", node_id);

        let mut members = self.members.write().await;

        if !members.contains_key(&node_id) {
            return Err(ClusterError::NodeNotFound(node_id));
        }

        members.remove(&node_id);
        drop(members);

        // Rebalance placement groups after node removal
        self.rebalance().await?;

        info!("Node {} removed successfully", node_id);
        Ok(())
    }

    /// Get the placement group for a given bucket/key
    pub async fn get_pg_for_key(&self, bucket: &str, key: &str) -> PlacementGroup {
        let hash = hash_key(bucket, key);
        let pg_id = (hash % self.num_placement_groups as u64) as u32;

        let placement_groups = self.placement_groups.read().await;
        placement_groups[pg_id as usize].clone()
    }

    /// Rebalance placement groups across all nodes
    pub async fn rebalance(&self) -> Result<(), ClusterError> {
        info!("Rebalancing placement groups");

        let members = self.members.read().await;
        let node_ids: Vec<NodeId> = members.keys().copied().collect();

        if node_ids.is_empty() {
            warn!("No nodes available for placement groups");
            return Ok(());
        }

        let mut placement_groups = self.placement_groups.write().await;

        for (i, pg) in placement_groups.iter_mut().enumerate() {
            // Use consistent hashing to assign nodes to placement groups
            pg.nodes = assign_nodes_to_pg(i as u32, &node_ids, 3); // 3 replicas
            debug!("PG {} assigned to nodes {:?}", pg.id, pg.nodes);
        }

        info!("Rebalancing complete");
        Ok(())
    }

    /// Get all cluster members
    pub async fn get_members(&self) -> Vec<NodeInfo> {
        let members = self.members.read().await;
        members.values().cloned().collect()
    }

    /// Get information about a specific node
    pub async fn get_node(&self, node_id: NodeId) -> Result<NodeInfo, ClusterError> {
        let members = self.members.read().await;
        members
            .get(&node_id)
            .cloned()
            .ok_or(ClusterError::NodeNotFound(node_id))
    }

    /// Check if this node is the cluster leader
    pub async fn is_leader(&self, node_id: NodeId) -> bool {
        self.get_leader().await == Some(node_id)
    }
}

/// Hash a bucket/key combination to a u64
fn hash_key(bucket: &str, key: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    bucket.hash(&mut hasher);
    key.hash(&mut hasher);
    hasher.finish()
}

/// Assign nodes to a placement group using consistent hashing
fn assign_nodes_to_pg(pg_id: u32, nodes: &[NodeId], replication_factor: usize) -> Vec<NodeId> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    if nodes.is_empty() {
        return Vec::new();
    }

    let mut node_scores: Vec<(NodeId, u64)> = nodes
        .iter()
        .map(|&node_id| {
            let mut hasher = DefaultHasher::new();
            pg_id.hash(&mut hasher);
            node_id.hash(&mut hasher);
            let score = hasher.finish();
            (node_id, score)
        })
        .collect();

    // Sort by hash score for consistent ordering
    node_scores.sort_by_key(|(_, score)| *score);

    // Take the top N nodes (up to replication factor)
    node_scores
        .into_iter()
        .take(replication_factor.min(nodes.len()))
        .map(|(node_id, _)| node_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_key() {
        let hash1 = hash_key("bucket1", "key1");
        let hash2 = hash_key("bucket1", "key1");
        let hash3 = hash_key("bucket1", "key2");

        // Same bucket/key should produce same hash
        assert_eq!(hash1, hash2);

        // Different key should produce different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_assign_nodes_to_pg() {
        let nodes = vec![1, 2, 3, 4, 5];

        let pg0 = assign_nodes_to_pg(0, &nodes, 3);
        let pg1 = assign_nodes_to_pg(1, &nodes, 3);

        // Should assign 3 nodes
        assert_eq!(pg0.len(), 3);
        assert_eq!(pg1.len(), 3);

        // Different PGs should potentially have different nodes
        // (not guaranteed, but likely with 5 nodes)
    }

    #[test]
    fn test_assign_nodes_with_fewer_than_replication_factor() {
        let nodes = vec![1, 2];

        let pg = assign_nodes_to_pg(0, &nodes, 3);

        // Should only assign available nodes
        assert_eq!(pg.len(), 2);
    }
}
