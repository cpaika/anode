use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use anode_raft::AnodeRaft;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

use super::discovery::{Discovery, NodeInfo};
use crate::cluster::NodeId;

#[derive(Error, Debug)]
pub enum MembershipError {
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

    #[error("Discovery error: {0}")]
    DiscoveryError(String),

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Timeout waiting for operation")]
    Timeout,
}

impl From<openraft::error::RaftError<NodeId>> for MembershipError {
    fn from(err: openraft::error::RaftError<NodeId>) -> Self {
        MembershipError::RaftError(err.to_string())
    }
}

impl From<openraft::error::CheckIsLeaderError<NodeId>> for MembershipError {
    fn from(err: openraft::error::CheckIsLeaderError<NodeId>) -> Self {
        MembershipError::RaftError(err.to_string())
    }
}

impl From<openraft::error::ClientWriteError<NodeId>> for MembershipError {
    fn from(err: openraft::error::ClientWriteError<NodeId>) -> Self {
        MembershipError::RaftError(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node_id: NodeId,
    pub status: HealthStatus,
    pub last_seen: SystemTime,
    pub response_time_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Cluster membership management
pub struct MembershipManager {
    raft: Arc<AnodeRaft>,
    discovery: Arc<Discovery>,
    members: RwLock<HashMap<NodeId, NodeInfo>>,
    health: RwLock<HashMap<NodeId, NodeHealth>>,
}

impl MembershipManager {
    pub fn new(raft: Arc<AnodeRaft>, discovery: Arc<Discovery>) -> Self {
        Self {
            raft,
            discovery,
            members: RwLock::new(HashMap::new()),
            health: RwLock::new(HashMap::new()),
        }
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, node: NodeInfo) -> Result<(), MembershipError> {
        info!("Adding node {} to cluster", node.id);

        // Check if we're the leader
        if !self.is_leader().await {
            warn!("Not the leader, cannot add node");
            return Err(MembershipError::NotLeader);
        }

        // Check if node already exists
        {
            let members = self.members.read().await;
            if members.contains_key(&node.id) {
                return Err(MembershipError::NodeAlreadyExists(node.id));
            }
        }

        // Add to raft cluster as learner first
        let raft_node = anode_raft::network::Node {
            addr: node.grpc_addr.clone(),
        };

        // Add as learner
        self.raft
            .add_learner(node.id, raft_node.clone(), true)
            .await?;

        info!("Node {} added as learner", node.id);

        // TODO: Wait for log sync before promoting to voter
        // For now, immediately promote to voter

        // Change membership to include new node as voter
        let mut new_members = std::collections::BTreeSet::new();
        let metrics = self.raft.metrics().borrow().clone();

        for voter_id in metrics.membership_config.membership().voter_ids() {
            new_members.insert(*voter_id);
        }
        new_members.insert(node.id);

        self.raft.change_membership(new_members, true).await?;

        // Update local membership cache
        {
            let mut members = self.members.write().await;
            members.insert(node.id, node.clone());
        }

        // Initialize health status
        {
            let mut health = self.health.write().await;
            health.insert(
                node.id,
                NodeHealth {
                    node_id: node.id,
                    status: HealthStatus::Unknown,
                    last_seen: SystemTime::now(),
                    response_time_ms: None,
                },
            );
        }

        info!("Node {} successfully added to cluster", node.id);
        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: NodeId) -> Result<(), MembershipError> {
        info!("Removing node {} from cluster", node_id);

        // Check if we're the leader
        if !self.is_leader().await {
            warn!("Not the leader, cannot remove node");
            return Err(MembershipError::NotLeader);
        }

        // Check if node exists
        {
            let members = self.members.read().await;
            if !members.contains_key(&node_id) {
                return Err(MembershipError::NodeNotFound(node_id));
            }
        }

        // Remove from raft cluster
        let metrics = self.raft.metrics().borrow().clone();
        let mut new_members = std::collections::BTreeSet::new();

        for voter_id in metrics.membership_config.membership().voter_ids() {
            if *voter_id != node_id {
                new_members.insert(*voter_id);
            }
        }

        self.raft.change_membership(new_members, true).await?;

        // Update local membership cache
        {
            let mut members = self.members.write().await;
            members.remove(&node_id);
        }

        // Remove health status
        {
            let mut health = self.health.write().await;
            health.remove(&node_id);
        }

        info!("Node {} successfully removed from cluster", node_id);
        Ok(())
    }

    /// Get current cluster members
    pub async fn get_members(&self) -> Vec<NodeInfo> {
        let members = self.members.read().await;
        members.values().cloned().collect()
    }

    /// Get current leader
    pub async fn get_leader(&self) -> Option<NodeId> {
        self.raft.current_leader().await
    }

    /// Check if we are the leader
    pub async fn is_leader(&self) -> bool {
        self.raft.current_leader().await.is_some()
            && self.raft.metrics().borrow().state == openraft::State::Leader
    }

    /// Handle node failure detection
    pub async fn handle_node_failure(&self, node_id: NodeId) -> Result<(), MembershipError> {
        warn!("Handling failure of node {}", node_id);

        // Update health status
        {
            let mut health = self.health.write().await;
            if let Some(node_health) = health.get_mut(&node_id) {
                node_health.status = HealthStatus::Unhealthy;
                node_health.last_seen = SystemTime::now();
            }
        }

        // If we're the leader, we might want to remove the failed node
        // In production, you'd want more sophisticated failure detection
        // and possibly a grace period before removal

        if self.is_leader().await {
            info!("Leader detected node {} failure, considering removal", node_id);
            // TODO: Implement more sophisticated failure handling
            // For now, we don't automatically remove nodes
        }

        Ok(())
    }

    /// Periodic health check of members
    pub async fn health_check(&self) -> Vec<NodeHealth> {
        debug!("Running health check on cluster members");

        let members = self.members.read().await;
        let mut results = Vec::new();

        for (node_id, node_info) in members.iter() {
            let health = self.check_node_health(*node_id, node_info).await;
            results.push(health.clone());

            // Update health cache
            let mut health_map = self.health.write().await;
            health_map.insert(*node_id, health);
        }

        debug!("Health check complete: {} nodes checked", results.len());
        results
    }

    /// Check health of a single node
    async fn check_node_health(&self, node_id: NodeId, node_info: &NodeInfo) -> NodeHealth {
        let start = std::time::Instant::now();

        // Try to connect to the node
        // In a real implementation, you'd make a gRPC health check call
        // For now, we'll just return a placeholder

        let status = if self.raft.current_leader().await == Some(node_id) {
            HealthStatus::Healthy
        } else {
            // Check if node is in the current membership
            let metrics = self.raft.metrics().borrow().clone();
            let is_member = metrics
                .membership_config
                .membership()
                .voter_ids()
                .any(|id| *id == node_id);

            if is_member {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unknown
            }
        };

        let response_time = start.elapsed().as_millis() as u64;

        NodeHealth {
            node_id,
            status,
            last_seen: SystemTime::now(),
            response_time_ms: Some(response_time),
        }
    }

    /// Sync membership from discovery
    pub async fn sync_from_discovery(&self) -> Result<(), MembershipError> {
        info!("Syncing membership from discovery");

        let discovered = self.discovery.discover_all().await.map_err(|e| {
            MembershipError::DiscoveryError(e.to_string())
        })?;

        debug!("Discovered {} nodes", discovered.len());

        // Update local membership cache with discovered nodes
        let mut members = self.members.write().await;
        for node in discovered {
            members.insert(node.id, node);
        }

        Ok(())
    }

    /// Get node by ID
    pub async fn get_node(&self, node_id: NodeId) -> Option<NodeInfo> {
        let members = self.members.read().await;
        members.get(&node_id).cloned()
    }

    /// Get health status for all nodes
    pub async fn get_health_status(&self) -> HashMap<NodeId, NodeHealth> {
        let health = self.health.read().await;
        health.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anode_raft::{RaftStorage, StateMachine, Network};
    use tempfile::TempDir;

    async fn create_test_membership() -> (MembershipManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();

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

        // Initialize as single-node cluster
        let mut members = std::collections::BTreeMap::new();
        members.insert(
            1,
            anode_raft::network::Node {
                addr: "127.0.0.1:6001".to_string(),
            },
        );
        Arc::new(raft).initialize(members).await.unwrap();

        let discovery = Arc::new(Discovery::new());
        let manager = MembershipManager::new(Arc::new(raft), discovery);

        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_get_members() {
        let (manager, _temp) = create_test_membership().await;

        let members = manager.get_members().await;
        // Initially empty because we haven't synced
        assert!(members.is_empty());
    }

    #[tokio::test]
    async fn test_health_status() {
        let (manager, _temp) = create_test_membership().await;

        let health = manager.get_health_status().await;
        // Initially empty
        assert!(health.is_empty());
    }

    #[tokio::test]
    async fn test_is_leader() {
        let (manager, _temp) = create_test_membership().await;

        // Give it time to elect leader
        tokio::time::sleep(Duration::from_millis(500)).await;

        // In a single-node cluster, we should be the leader
        let is_leader = manager.is_leader().await;
        assert!(is_leader);
    }

    #[tokio::test]
    async fn test_get_leader() {
        let (manager, _temp) = create_test_membership().await;

        // Give it time to elect leader
        tokio::time::sleep(Duration::from_millis(500)).await;

        let leader = manager.get_leader().await;
        assert_eq!(leader, Some(1));
    }

    #[test]
    fn test_health_status_enum() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    }
}
