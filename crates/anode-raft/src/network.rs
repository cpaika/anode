use crate::config::TlsConfig;
use crate::types::{NodeId, TypeConfig};
use openraft::error::{InstallSnapshotError, NetworkError, RPCError, RaftError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::BasicNode;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

/// Network manager for creating Raft RPC connections to cluster nodes.
///
/// This type implements [`openraft::network::RaftNetworkFactory`] and is responsible
/// for managing network connections between Raft nodes. It maintains a registry of
/// cluster nodes and caches gRPC connections for efficient communication.
///
/// # TLS Support
///
/// The network manager supports optional TLS/mTLS for secure inter-node communication.
/// When TLS is enabled, all connections are encrypted and can optionally require
/// mutual authentication via client certificates.
///
/// # Implementation Status
///
/// Currently, this is a placeholder implementation. The actual gRPC service
/// definitions are not yet integrated. The RPC methods return `Unreachable` errors
/// until the protobuf definitions from `anode-protocol` are connected.
///
/// # Examples
///
/// ```
/// use anode_raft::RaftNetworkManager;
/// use openraft::BasicNode;
///
/// let manager = RaftNetworkManager::new();
///
/// // Register a node
/// manager.add_node(1, BasicNode {
///     addr: "127.0.0.1:8080".to_string(),
/// });
///
/// // Remove a node
/// manager.remove_node(1);
/// ```
#[derive(Clone)]
pub struct RaftNetworkManager {
    /// Map of node IDs to their network addresses
    nodes: Arc<RwLock<HashMap<NodeId, BasicNode>>>,
    /// Cached gRPC connections for efficient reuse
    connections: Arc<RwLock<HashMap<NodeId, Channel>>>,
    /// Optional TLS configuration for secure connections
    tls_config: Option<Arc<TlsConfig>>,
}

impl RaftNetworkManager {
    /// Create a new network manager with no registered nodes and no TLS.
    ///
    /// Use [`add_node`](Self::add_node) to register cluster members.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftNetworkManager;
    ///
    /// let manager = RaftNetworkManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            tls_config: None,
        }
    }

    /// Create a new network manager with TLS enabled.
    ///
    /// All connections will use TLS. If `tls_config.require_client_cert` is true,
    /// mutual TLS authentication will be enforced.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::{RaftNetworkManager, TlsConfig};
    /// use std::path::PathBuf;
    ///
    /// let tls_config = TlsConfig {
    ///     enabled: true,
    ///     cert_file: PathBuf::from("/etc/anode/tls/node.crt"),
    ///     key_file: PathBuf::from("/etc/anode/tls/node.key"),
    ///     ca_file: Some(PathBuf::from("/etc/anode/tls/ca.crt")),
    ///     require_client_cert: true,
    ///     verify_certificates: true,
    /// };
    ///
    /// let manager = RaftNetworkManager::with_tls(tls_config);
    /// ```
    pub fn with_tls(tls_config: TlsConfig) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            tls_config: Some(Arc::new(tls_config)),
        }
    }

    /// Register a node with its network address.
    ///
    /// This makes the node available for Raft RPC calls. The address should be
    /// in the format `host:port` (e.g., `"127.0.0.1:8080"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftNetworkManager;
    /// use openraft::BasicNode;
    ///
    /// let manager = RaftNetworkManager::new();
    /// manager.add_node(1, BasicNode {
    ///     addr: "192.168.1.10:8080".to_string(),
    /// });
    /// ```
    pub fn add_node(&self, node_id: NodeId, node: BasicNode) {
        self.nodes.write().insert(node_id, node);
    }

    /// Remove a node from the registry.
    ///
    /// This also closes and removes any cached connection to the node.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftNetworkManager;
    /// use openraft::BasicNode;
    ///
    /// let manager = RaftNetworkManager::new();
    /// manager.add_node(1, BasicNode {
    ///     addr: "127.0.0.1:8080".to_string(),
    /// });
    ///
    /// manager.remove_node(1);
    /// ```
    pub fn remove_node(&self, node_id: NodeId) {
        self.nodes.write().remove(&node_id);
        self.connections.write().remove(&node_id);
    }

    /// Get the number of registered nodes (for testing).
    ///
    /// This is primarily useful for testing and monitoring.
    pub fn node_count(&self) -> usize {
        self.nodes.read().len()
    }

    /// Check if a node is registered (for testing).
    ///
    /// Returns `true` if the node with the given ID is registered.
    pub fn has_node(&self, node_id: NodeId) -> bool {
        self.nodes.read().contains_key(&node_id)
    }

    /// Get the number of cached connections (for testing).
    ///
    /// This is primarily useful for testing and monitoring.
    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }

    /// Check if TLS is configured.
    ///
    /// Returns `true` if this network manager was created with TLS configuration.
    pub fn has_tls(&self) -> bool {
        self.tls_config.is_some()
    }

    /// Get TLS configuration.
    ///
    /// Returns the TLS configuration if one was provided, or `None` otherwise.
    pub fn get_tls_config(&self) -> Option<&TlsConfig> {
        self.tls_config.as_ref().map(|arc| arc.as_ref())
    }

    /// Get or create a gRPC connection to the specified node.
    ///
    /// Connections are cached for reuse. If a connection already exists, it is
    /// returned. Otherwise, a new connection is established.
    ///
    /// When TLS is enabled, the connection will use TLS with optional client
    /// certificate authentication (mTLS).
    ///
    /// # Errors
    ///
    /// Returns [`NetworkError`] if:
    /// - The node is not registered
    /// - The node's address is invalid
    /// - Connection establishment fails
    /// - TLS configuration is invalid
    pub async fn get_connection(&self, node_id: NodeId) -> Result<Channel, NetworkError> {
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
            nodes.get(&node_id).cloned().ok_or_else(|| {
                let err = std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Node {} not found", node_id),
                );
                NetworkError::new(&err)
            })?
        };

        // Determine protocol based on TLS configuration
        let (protocol, tls_config) = if let Some(ref tls_cfg) = self.tls_config {
            if tls_cfg.enabled {
                ("https", Some(tls_cfg.clone()))
            } else {
                ("http", None)
            }
        } else {
            ("http", None)
        };

        // Create endpoint
        let endpoint =
            Endpoint::from_shared(format!("{}://{}", protocol, node.addr)).map_err(|e| {
                let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, e);
                NetworkError::new(&err)
            })?;

        // Configure TLS if enabled
        let endpoint = if let Some(tls_cfg) = tls_config {
            let client_tls = self.build_client_tls_config(&tls_cfg).map_err(|e| {
                let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, e);
                NetworkError::new(&err)
            })?;
            endpoint.tls_config(client_tls).map_err(|e| {
                let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, e);
                NetworkError::new(&err)
            })?
        } else {
            endpoint
        };

        let channel = endpoint.connect().await.map_err(|e| {
            let err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e);
            NetworkError::new(&err)
        })?;

        // Cache the connection
        self.connections.write().insert(node_id, channel.clone());

        Ok(channel)
    }

    /// Build tonic ClientTlsConfig from our TlsConfig.
    ///
    /// This configures client-side TLS including optional client certificate
    /// authentication for mTLS.
    fn build_client_tls_config(&self, tls_cfg: &TlsConfig) -> Result<ClientTlsConfig, String> {
        let mut client_tls = ClientTlsConfig::new();

        // Load CA certificate if provided for server verification
        if let Some(ref ca_file) = tls_cfg.ca_file {
            let ca_cert = std::fs::read(ca_file)
                .map_err(|e| format!("Failed to read CA certificate: {}", e))?;
            client_tls =
                client_tls.ca_certificate(tonic::transport::Certificate::from_pem(ca_cert));
        }

        // Load client certificate and key if mTLS is required
        if tls_cfg.require_client_cert {
            let cert = std::fs::read(&tls_cfg.cert_file)
                .map_err(|e| format!("Failed to read client certificate: {}", e))?;
            let key = std::fs::read(&tls_cfg.key_file)
                .map_err(|e| format!("Failed to read client key: {}", e))?;

            let identity = tonic::transport::Identity::from_pem(cert, key);
            client_tls = client_tls.identity(identity);
        }

        Ok(client_tls)
    }
}

impl Default for RaftNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Network connection to a specific Raft node.
///
/// This type implements [`openraft::network::RaftNetwork`] and handles RPC
/// calls to a single target node. It is created by [`RaftNetworkManager`]
/// when needed.
///
/// # Implementation Status
///
/// The RPC methods currently return placeholder errors until the gRPC
/// service is fully integrated. See the inline comments in the method
/// implementations for the intended integration points.
pub struct RaftNetworkConnection {
    /// The target node ID for this connection
    target: NodeId,
    /// Reference to the parent network manager
    manager: RaftNetworkManager,
}

impl RaftNetwork<TypeConfig> for RaftNetworkConnection {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
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
        let err = std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "gRPC service not yet implemented",
        );
        Err(RPCError::Unreachable(Unreachable::new(&NetworkError::new(
            &err,
        ))))
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        tracing::debug!(
            "Sending Vote request to node {}: term {}",
            self.target,
            rpc.vote.leader_id.term
        );

        // Similar to append_entries, this would use gRPC in production
        // let channel = self.manager.get_connection(self.target).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(e)))?;
        // let mut client = RaftServiceClient::new(channel);
        // let response = client.vote(rpc).await
        //     .map_err(|e| RPCError::Network(NetworkError::from(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
        // Ok(response.into_inner())

        let err = std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "gRPC service not yet implemented",
        );
        Err(RPCError::Unreachable(Unreachable::new(&NetworkError::new(
            &err,
        ))))
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
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

        let err = std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "gRPC service not yet implemented",
        );
        Err(RPCError::Unreachable(Unreachable::new(&NetworkError::new(
            &err,
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
