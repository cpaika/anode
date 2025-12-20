use crate::error::RaftError;
use crate::types::{NodeId, TypeConfig};
use openraft::error::{NetworkError, RPCError, RemoteError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::BasicNode;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::transport::{Channel, Endpoint};

/// Network factory for creating Raft network connections
#[derive(Clone)]
pub struct RaftNetworkManager {
    /// Map of node IDs to their network addresses
    nodes: Arc<RwLock<HashMap<NodeId, BasicNode>>>,
    /// Cached gRPC connections
    connections: Arc<RwLock<HashMap<NodeId, Channel>>>,
}

impl RaftNetworkManager {
    /// Create a new network factory
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a node with its address
    pub fn add_node(&self, node_id: NodeId, node: BasicNode) {
        self.nodes.write().insert(node_id, node);
    }

    /// Remove a node
    pub fn remove_node(&self, node_id: NodeId) {
        self.nodes.write().remove(&node_id);
        self.connections.write().remove(&node_id);
    }

    /// Get a connection to a node, creating one if necessary
    async fn get_connection(&self, node_id: NodeId) -> Result<Channel, NetworkError> {
        // Check if we already have a connection
        {
            let connections = self.connections.read();
            if let Some(channel) = connections.get(&node_id) {
                return Ok(channel.clone());
            }
        }

        // Get node address
        let node = {
            let nodes = self.nodes.read();
            nodes
                .get(&node_id)
                .cloned()
                .ok_or_else(|| NetworkError::from(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Node {} not found", node_id),
                )))?
        };

        // Create new connection
        let endpoint = Endpoint::from_shared(format!("http://{}", node.addr))
            .map_err(|e| NetworkError::from(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| NetworkError::from(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e)))?;

        // Cache the connection
        self.connections.write().insert(node_id, channel.clone());

        Ok(channel)
    }
}

impl Default for RaftNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RaftNetworkFactory<TypeConfig> for RaftNetworkManager {
    type Network = RaftNetworkConnection;

    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        // Ensure the node is registered
        self.add_node(target, node.clone());

        RaftNetworkConnection {
            target,
            manager: self.clone(),
        }
    }
}

/// Network connection for a specific target node
pub struct RaftNetworkConnection {
    target: NodeId,
    manager: RaftNetworkManager,
}

#[async_trait::async_trait]
impl RaftNetwork<TypeConfig> for RaftNetworkConnection {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError>> {
        tracing::debug!(
            "Sending AppendEntries to node {}: {} entries",
            self.target,
            rpc.entries.len()
        );

        // In a real implementation, we would use the gRPC client to send the request
        // For now, we simulate a network error since we don't have the gRPC service defined
        // This is where you'd integrate with the proto-generated RaftService client

        // Example of what the real implementation would look like:
        // let channel = self.manager.get_connection(self.target).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(e)))?;
        // let mut client = RaftServiceClient::new(channel);
        // let response = client.append_entries(rpc).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
        // Ok(response.into_inner())

        // Placeholder implementation
        Err(RPCError::Unreachable(Unreachable::new(&NetworkError::from(
            std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "gRPC service not yet implemented",
            ),
        ))))
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError>> {
        tracing::debug!(
            "Sending Vote request to node {}: term {}",
            self.target,
            rpc.vote.term
        );

        // Similar to append_entries, this would use gRPC in production
        // let channel = self.manager.get_connection(self.target).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(e)))?;
        // let mut client = RaftServiceClient::new(channel);
        // let response = client.vote(rpc).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
        // Ok(response.into_inner())

        Err(RPCError::Unreachable(Unreachable::new(&NetworkError::from(
            std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "gRPC service not yet implemented",
            ),
        ))))
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError>> {
        tracing::debug!(
            "Sending InstallSnapshot to node {}: offset {}",
            self.target,
            rpc.offset
        );

        // Similar to above, would use gRPC streaming in production
        // let channel = self.manager.get_connection(self.target).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(e)))?;
        // let mut client = RaftServiceClient::new(channel);
        //
        // // Stream the snapshot data in chunks
        // let stream = stream_snapshot_data(rpc);
        // let response = client.install_snapshot(stream).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
        // Ok(response.into_inner())

        Err(RPCError::Unreachable(Unreachable::new(&NetworkError::from(
            std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "gRPC service not yet implemented",
            ),
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_factory_creation() {
        let factory = RaftNetworkManager::new();
        assert_eq!(factory.nodes.read().len(), 0);
    }

    #[test]
    fn test_add_and_remove_node() {
        let factory = RaftNetworkManager::new();

        let node = BasicNode {
            addr: "127.0.0.1:8080".to_string(),
        };

        factory.add_node(1, node.clone());
        assert_eq!(factory.nodes.read().len(), 1);
        assert_eq!(factory.nodes.read().get(&1).unwrap().addr, node.addr);

        factory.remove_node(1);
        assert_eq!(factory.nodes.read().len(), 0);
    }

    #[tokio::test]
    async fn test_get_connection_not_found() {
        let factory = RaftNetworkManager::new();
        let result = factory.get_connection(999).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_new_client() {
        let mut factory = RaftNetworkManager::new();
        let node = BasicNode {
            addr: "127.0.0.1:8080".to_string(),
        };

        let _connection = factory.new_client(1, &node).await;
        assert_eq!(factory.nodes.read().len(), 1);
    }
}
