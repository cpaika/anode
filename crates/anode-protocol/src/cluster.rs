use crate::error::ProtocolResult;
use crate::proto;
use std::sync::Arc;
use tonic::{Request, Response, Status};

/// Cluster management service server implementation
///
/// This service handles cluster membership operations including nodes
/// joining/leaving the cluster and retrieving cluster status.
pub struct ClusterServiceImpl<C> {
    cluster_manager: Arc<C>,
}

impl<C> ClusterServiceImpl<C> {
    pub fn new(cluster_manager: Arc<C>) -> Self {
        Self { cluster_manager }
    }
}

/// Trait that must be implemented by the cluster manager
///
/// This trait abstracts the actual cluster management implementation from the gRPC service layer.
#[tonic::async_trait]
pub trait ClusterManager: Send + Sync + 'static {
    /// Handle node join request
    async fn join(
        &self,
        request: proto::cluster::JoinRequest,
    ) -> ProtocolResult<proto::cluster::JoinResponse>;

    /// Handle node leave request
    async fn leave(
        &self,
        request: proto::cluster::LeaveRequest,
    ) -> ProtocolResult<proto::cluster::LeaveResponse>;

    /// Get cluster status
    async fn get_cluster_status(
        &self,
        request: proto::cluster::GetClusterStatusRequest,
    ) -> ProtocolResult<proto::cluster::ClusterStatus>;

    /// Get status of a specific node
    async fn get_node_status(
        &self,
        request: proto::cluster::GetNodeStatusRequest,
    ) -> ProtocolResult<proto::cluster::NodeStatus>;
}

#[tonic::async_trait]
impl<C: ClusterManager> proto::cluster::cluster_service_server::ClusterService
    for ClusterServiceImpl<C>
{
    /// Handle node join request
    ///
    /// A new node calls this on an existing cluster member to join the cluster.
    /// The request includes the node's ID and address.
    async fn join(
        &self,
        request: Request<proto::cluster::JoinRequest>,
    ) -> Result<Response<proto::cluster::JoinResponse>, Status> {
        let req = request.into_inner();

        tracing::info!(
            node_id = req.node_id.as_ref().map(|id| id.id).unwrap_or(0),
            address = %req.address,
            "Received Join request"
        );

        let response = self
            .cluster_manager
            .join(req)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Join request failed");
                Status::from(e)
            })?;

        tracing::info!(
            success = response.success,
            error = %response.error,
            "Sending Join response"
        );

        Ok(Response::new(response))
    }

    /// Handle node leave request
    ///
    /// A node calls this to gracefully leave the cluster. The cluster will
    /// redistribute data and update membership accordingly.
    async fn leave(
        &self,
        request: Request<proto::cluster::LeaveRequest>,
    ) -> Result<Response<proto::cluster::LeaveResponse>, Status> {
        let req = request.into_inner();

        tracing::info!(
            node_id = req.node_id.as_ref().map(|id| id.id).unwrap_or(0),
            "Received Leave request"
        );

        let response = self
            .cluster_manager
            .leave(req)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Leave request failed");
                Status::from(e)
            })?;

        tracing::info!(
            success = response.success,
            error = %response.error,
            "Sending Leave response"
        );

        Ok(Response::new(response))
    }

    /// Get cluster status
    ///
    /// Returns information about all nodes in the cluster, placement groups,
    /// and the current leader.
    async fn get_cluster_status(
        &self,
        request: Request<proto::cluster::GetClusterStatusRequest>,
    ) -> Result<Response<proto::cluster::ClusterStatus>, Status> {
        let req = request.into_inner();

        tracing::debug!("Received GetClusterStatus request");

        let response = self
            .cluster_manager
            .get_cluster_status(req)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "GetClusterStatus request failed");
                Status::from(e)
            })?;

        tracing::debug!(
            cluster_id = response.cluster_id.as_ref().map(|id| &id.id).unwrap_or(&String::new()),
            nodes_count = response.nodes.len(),
            placement_groups_count = response.placement_groups.len(),
            "Sending GetClusterStatus response"
        );

        Ok(Response::new(response))
    }

    /// Get node status
    ///
    /// Returns detailed status information for a specific node, including
    /// disk usage, object count, and current state.
    async fn get_node_status(
        &self,
        request: Request<proto::cluster::GetNodeStatusRequest>,
    ) -> Result<Response<proto::cluster::NodeStatus>, Status> {
        let req = request.into_inner();

        tracing::debug!(
            node_id = req.node_id.as_ref().map(|id| id.id).unwrap_or(0),
            "Received GetNodeStatus request"
        );

        let response = self
            .cluster_manager
            .get_node_status(req)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "GetNodeStatus request failed");
                Status::from(e)
            })?;

        tracing::debug!(
            node_id = response.node_id.as_ref().map(|id| id.id).unwrap_or(0),
            state = response.state,
            disk_used = response.disk_used,
            disk_total = response.disk_total,
            "Sending GetNodeStatus response"
        );

        Ok(Response::new(response))
    }
}

/// Helper functions for working with node states
pub mod node_state {
    use crate::proto::cluster::NodeState;

    /// Check if a node is in active state
    pub fn is_active(state: i32) -> bool {
        state == NodeState::Active as i32
    }

    /// Check if a node is joining
    pub fn is_joining(state: i32) -> bool {
        state == NodeState::Joining as i32
    }

    /// Check if a node is leaving
    pub fn is_leaving(state: i32) -> bool {
        state == NodeState::Leaving as i32
    }

    /// Check if a node is down
    pub fn is_down(state: i32) -> bool {
        state == NodeState::Down as i32
    }

    /// Convert NodeState enum to string
    pub fn to_string(state: i32) -> &'static str {
        match state {
            s if s == NodeState::Unknown as i32 => "UNKNOWN",
            s if s == NodeState::Joining as i32 => "JOINING",
            s if s == NodeState::Active as i32 => "ACTIVE",
            s if s == NodeState::Leaving as i32 => "LEAVING",
            s if s == NodeState::Down as i32 => "DOWN",
            _ => "UNKNOWN",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::cluster::cluster_service_server::ClusterService;
    use crate::proto::cluster::*;
    use crate::proto::common;

    struct MockClusterManager {
        cluster_id: String,
        node_count: usize,
    }

    #[tonic::async_trait]
    impl ClusterManager for MockClusterManager {
        async fn join(&self, request: JoinRequest) -> ProtocolResult<JoinResponse> {
            let success = request.node_id.is_some();
            Ok(JoinResponse {
                success,
                error: if success {
                    String::new()
                } else {
                    "Missing node ID".to_string()
                },
                cluster: if success {
                    Some(ClusterStatus {
                        cluster_id: Some(common::ClusterId {
                            id: self.cluster_id.clone(),
                        }),
                        nodes: vec![],
                        placement_groups: vec![],
                        leader: None,
                    })
                } else {
                    None
                },
            })
        }

        async fn leave(&self, request: LeaveRequest) -> ProtocolResult<LeaveResponse> {
            let success = request.node_id.is_some();
            Ok(LeaveResponse {
                success,
                error: if success {
                    String::new()
                } else {
                    "Missing node ID".to_string()
                },
            })
        }

        async fn get_cluster_status(
            &self,
            _request: GetClusterStatusRequest,
        ) -> ProtocolResult<ClusterStatus> {
            Ok(ClusterStatus {
                cluster_id: Some(common::ClusterId {
                    id: self.cluster_id.clone(),
                }),
                nodes: (1..=self.node_count)
                    .map(|i| NodeInfo {
                        node_id: Some(common::NodeId { id: i as u64 }),
                        address: format!("node{}:8080", i),
                        state: NodeState::Active as i32,
                    })
                    .collect(),
                placement_groups: vec![],
                leader: Some(common::NodeId { id: 1 }),
            })
        }

        async fn get_node_status(
            &self,
            request: GetNodeStatusRequest,
        ) -> ProtocolResult<NodeStatus> {
            Ok(NodeStatus {
                node_id: request.node_id,
                state: NodeState::Active as i32,
                disk_total: 1_000_000_000,
                disk_used: 500_000_000,
                objects_count: 1000,
                chunks_count: 5000,
            })
        }
    }

    #[tokio::test]
    async fn test_join_success() {
        let manager = Arc::new(MockClusterManager {
            cluster_id: "test-cluster".to_string(),
            node_count: 3,
        });
        let service = ClusterServiceImpl::new(manager);

        let request = Request::new(JoinRequest {
            node_id: Some(common::NodeId { id: 4 }),
            address: "node4:8080".to_string(),
        });

        let response = service.join(request).await.unwrap();
        let resp = response.into_inner();

        assert!(resp.success);
        assert!(resp.error.is_empty());
        assert!(resp.cluster.is_some());
    }

    #[tokio::test]
    async fn test_join_missing_node_id() {
        let manager = Arc::new(MockClusterManager {
            cluster_id: "test-cluster".to_string(),
            node_count: 3,
        });
        let service = ClusterServiceImpl::new(manager);

        let request = Request::new(JoinRequest {
            node_id: None,
            address: "node4:8080".to_string(),
        });

        let response = service.join(request).await.unwrap();
        let resp = response.into_inner();

        assert!(!resp.success);
        assert!(!resp.error.is_empty());
    }

    #[tokio::test]
    async fn test_leave_success() {
        let manager = Arc::new(MockClusterManager {
            cluster_id: "test-cluster".to_string(),
            node_count: 3,
        });
        let service = ClusterServiceImpl::new(manager);

        let request = Request::new(LeaveRequest {
            node_id: Some(common::NodeId { id: 3 }),
        });

        let response = service.leave(request).await.unwrap();
        let resp = response.into_inner();

        assert!(resp.success);
        assert!(resp.error.is_empty());
    }

    #[tokio::test]
    async fn test_get_cluster_status() {
        let manager = Arc::new(MockClusterManager {
            cluster_id: "test-cluster".to_string(),
            node_count: 3,
        });
        let service = ClusterServiceImpl::new(manager);

        let request = Request::new(GetClusterStatusRequest {});

        let response = service.get_cluster_status(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(
            resp.cluster_id.as_ref().unwrap().id,
            "test-cluster"
        );
        assert_eq!(resp.nodes.len(), 3);
        assert_eq!(resp.leader.as_ref().unwrap().id, 1);
    }

    #[tokio::test]
    async fn test_get_node_status() {
        let manager = Arc::new(MockClusterManager {
            cluster_id: "test-cluster".to_string(),
            node_count: 3,
        });
        let service = ClusterServiceImpl::new(manager);

        let request = Request::new(GetNodeStatusRequest {
            node_id: Some(common::NodeId { id: 2 }),
        });

        let response = service.get_node_status(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.node_id.as_ref().unwrap().id, 2);
        assert_eq!(resp.state, NodeState::Active as i32);
        assert_eq!(resp.disk_total, 1_000_000_000);
        assert_eq!(resp.disk_used, 500_000_000);
    }

    #[test]
    fn test_node_state_helpers() {
        assert!(node_state::is_active(NodeState::Active as i32));
        assert!(!node_state::is_active(NodeState::Down as i32));

        assert!(node_state::is_joining(NodeState::Joining as i32));
        assert!(!node_state::is_joining(NodeState::Active as i32));

        assert_eq!(
            node_state::to_string(NodeState::Active as i32),
            "ACTIVE"
        );
        assert_eq!(
            node_state::to_string(NodeState::Down as i32),
            "DOWN"
        );
    }
}
