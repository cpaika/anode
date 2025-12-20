use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tracing::{debug, info, warn};

use crate::cluster::NodeId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub name: String,
    pub grpc_addr: String,
    pub s3_addr: String,
    pub admin_addr: String,
}

impl NodeInfo {
    pub fn new(id: NodeId, name: String, grpc_addr: String) -> Self {
        Self {
            id,
            name: name.clone(),
            grpc_addr: grpc_addr.clone(),
            s3_addr: format!("http://{}", grpc_addr),
            admin_addr: format!("http://{}", grpc_addr),
        }
    }
}

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("No nodes discovered")]
    NoNodesFound,

    #[error("DNS lookup failed: {0}")]
    DnsError(String),

    #[error("Kubernetes API error: {0}")]
    KubernetesError(String),

    #[error("File read error: {0}")]
    FileError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

impl From<std::io::Error> for DiscoveryError {
    fn from(err: std::io::Error) -> Self {
        DiscoveryError::FileError(err.to_string())
    }
}

impl From<serde_json::Error> for DiscoveryError {
    fn from(err: serde_json::Error) -> Self {
        DiscoveryError::ParseError(err.to_string())
    }
}

#[async_trait]
pub trait DiscoveryMethod: Send + Sync {
    async fn discover(&self) -> Result<Vec<NodeInfo>, DiscoveryError>;
}

/// Node discovery for finding cluster members
pub struct Discovery {
    methods: Vec<Arc<dyn DiscoveryMethod>>,
}

impl Discovery {
    pub fn new() -> Self {
        Self {
            methods: Vec::new(),
        }
    }

    /// Add a discovery method
    pub fn add_method(&mut self, method: Arc<dyn DiscoveryMethod>) {
        self.methods.push(method);
    }

    /// Discover nodes using all configured methods
    pub async fn discover_all(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        if self.methods.is_empty() {
            warn!("No discovery methods configured");
            return Err(DiscoveryError::NoNodesFound);
        }

        let mut all_nodes = Vec::new();
        let mut errors = Vec::new();

        for method in &self.methods {
            match method.discover().await {
                Ok(nodes) => {
                    debug!("Discovered {} nodes", nodes.len());
                    all_nodes.extend(nodes);
                }
                Err(e) => {
                    warn!("Discovery method failed: {}", e);
                    errors.push(e);
                }
            }
        }

        // Deduplicate nodes by ID
        let mut unique_nodes = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for node in all_nodes {
            if seen_ids.insert(node.id) {
                unique_nodes.push(node);
            }
        }

        if unique_nodes.is_empty() {
            if !errors.is_empty() {
                return Err(errors.into_iter().next().unwrap());
            }
            return Err(DiscoveryError::NoNodesFound);
        }

        info!("Discovered {} unique nodes", unique_nodes.len());
        Ok(unique_nodes)
    }
}

impl Default for Discovery {
    fn default() -> Self {
        Self::new()
    }
}

/// Static list of nodes
pub struct StaticDiscovery {
    nodes: Vec<NodeInfo>,
}

impl StaticDiscovery {
    pub fn new(nodes: Vec<NodeInfo>) -> Self {
        Self { nodes }
    }

    pub fn from_addresses(addresses: Vec<String>) -> Self {
        let nodes = addresses
            .into_iter()
            .enumerate()
            .map(|(idx, addr)| NodeInfo::new(
                (idx + 1) as NodeId,
                format!("node-{}", idx + 1),
                addr,
            ))
            .collect();

        Self { nodes }
    }
}

#[async_trait]
impl DiscoveryMethod for StaticDiscovery {
    async fn discover(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        debug!("Static discovery: {} nodes configured", self.nodes.len());
        Ok(self.nodes.clone())
    }
}

/// DNS-based discovery (SRV records)
pub struct DnsDiscovery {
    service_name: String,
}

impl DnsDiscovery {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }
}

#[async_trait]
impl DiscoveryMethod for DnsDiscovery {
    async fn discover(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        debug!("DNS discovery for service: {}", self.service_name);

        // Use tokio's DNS resolver
        // This is a simplified implementation - in production, you'd want to:
        // 1. Query SRV records specifically
        // 2. Handle multiple records
        // 3. Parse priority and weight

        match tokio::net::lookup_host(&self.service_name).await {
            Ok(addrs) => {
                let nodes: Vec<NodeInfo> = addrs
                    .enumerate()
                    .map(|(idx, addr)| NodeInfo::new(
                        (idx + 1) as NodeId,
                        format!("dns-node-{}", idx + 1),
                        addr.to_string(),
                    ))
                    .collect();

                info!("DNS discovery found {} nodes", nodes.len());
                Ok(nodes)
            }
            Err(e) => {
                warn!("DNS lookup failed: {}", e);
                Err(DiscoveryError::DnsError(e.to_string()))
            }
        }
    }
}

/// Kubernetes discovery via API
pub struct KubernetesDiscovery {
    namespace: String,
    service_name: String,
}

impl KubernetesDiscovery {
    pub fn new(namespace: String, service_name: String) -> Self {
        Self {
            namespace,
            service_name,
        }
    }

    /// Detect if running in Kubernetes
    pub fn is_in_cluster() -> bool {
        std::path::Path::new("/var/run/secrets/kubernetes.io/serviceaccount").exists()
    }
}

#[async_trait]
impl DiscoveryMethod for KubernetesDiscovery {
    async fn discover(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        debug!(
            "Kubernetes discovery: namespace={}, service={}",
            self.namespace, self.service_name
        );

        // In a real implementation, this would:
        // 1. Read service account token from /var/run/secrets/kubernetes.io/serviceaccount/token
        // 2. Query Kubernetes API for endpoints
        // 3. Parse the endpoint list
        // 4. Return node information

        // For now, return a placeholder
        // You would typically use the k8s-openapi crate for this

        if !Self::is_in_cluster() {
            return Err(DiscoveryError::KubernetesError(
                "Not running in Kubernetes cluster".to_string(),
            ));
        }

        // Placeholder implementation
        warn!("Kubernetes discovery not fully implemented");
        Err(DiscoveryError::KubernetesError(
            "Not implemented".to_string(),
        ))
    }
}

/// File-based discovery (watched file)
pub struct FileDiscovery {
    path: PathBuf,
}

impl FileDiscovery {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl DiscoveryMethod for FileDiscovery {
    async fn discover(&self) -> Result<Vec<NodeInfo>, DiscoveryError> {
        debug!("File discovery from: {}", self.path.display());

        if !self.path.exists() {
            warn!("Discovery file does not exist: {}", self.path.display());
            return Err(DiscoveryError::FileError(format!(
                "File not found: {}",
                self.path.display()
            )));
        }

        let content = fs::read_to_string(&self.path).await?;
        let nodes: Vec<NodeInfo> = serde_json::from_str(&content)?;

        info!("File discovery found {} nodes", nodes.len());
        Ok(nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_discovery() {
        let nodes = vec![
            NodeInfo::new(1, "node1".to_string(), "127.0.0.1:6001".to_string()),
            NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string()),
        ];

        let discovery = StaticDiscovery::new(nodes.clone());
        let result = discovery.discover().await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
    }

    #[tokio::test]
    async fn test_static_discovery_from_addresses() {
        let addresses = vec![
            "127.0.0.1:6001".to_string(),
            "127.0.0.1:6002".to_string(),
            "127.0.0.1:6003".to_string(),
        ];

        let discovery = StaticDiscovery::from_addresses(addresses);
        let result = discovery.discover().await.unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
        assert_eq!(result[2].id, 3);
    }

    #[tokio::test]
    async fn test_discovery_multiple_methods() {
        let mut discovery = Discovery::new();

        let nodes1 = vec![
            NodeInfo::new(1, "node1".to_string(), "127.0.0.1:6001".to_string()),
        ];

        let nodes2 = vec![
            NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string()),
        ];

        discovery.add_method(Arc::new(StaticDiscovery::new(nodes1)));
        discovery.add_method(Arc::new(StaticDiscovery::new(nodes2)));

        let result = discovery.discover_all().await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_discovery_deduplication() {
        let mut discovery = Discovery::new();

        let nodes1 = vec![
            NodeInfo::new(1, "node1".to_string(), "127.0.0.1:6001".to_string()),
        ];

        let nodes2 = vec![
            NodeInfo::new(1, "node1-dup".to_string(), "127.0.0.1:6001".to_string()),
        ];

        discovery.add_method(Arc::new(StaticDiscovery::new(nodes1)));
        discovery.add_method(Arc::new(StaticDiscovery::new(nodes2)));

        let result = discovery.discover_all().await.unwrap();
        // Should deduplicate by ID
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[tokio::test]
    async fn test_discovery_no_methods() {
        let discovery = Discovery::new();
        let result = discovery.discover_all().await;
        assert!(matches!(result, Err(DiscoveryError::NoNodesFound)));
    }

    #[tokio::test]
    async fn test_file_discovery_missing_file() {
        let discovery = FileDiscovery::new(PathBuf::from("/nonexistent/file.json"));
        let result = discovery.discover().await;
        assert!(matches!(result, Err(DiscoveryError::FileError(_))));
    }

    #[tokio::test]
    async fn test_file_discovery() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let nodes = vec![
            NodeInfo::new(1, "node1".to_string(), "127.0.0.1:6001".to_string()),
            NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string()),
        ];

        let mut temp_file = NamedTempFile::new().unwrap();
        let json = serde_json::to_string(&nodes).unwrap();
        temp_file.write_all(json.as_bytes()).unwrap();

        let discovery = FileDiscovery::new(temp_file.path().to_path_buf());
        let result = discovery.discover().await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
    }

    #[test]
    fn test_kubernetes_is_in_cluster() {
        // This will be false in test environment
        let in_cluster = KubernetesDiscovery::is_in_cluster();
        assert!(!in_cluster);
    }
}
