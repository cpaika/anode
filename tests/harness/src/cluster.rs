//! Multi-node test cluster management
//!
//! Provides infrastructure for spawning, managing, and coordinating
//! multiple anode nodes in a test environment.

use crate::{NodeId, Result, TestHarnessError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Configuration for a single test node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique node identifier
    pub id: NodeId,
    /// S3 API port
    pub s3_port: u16,
    /// gRPC port for cluster communication
    pub grpc_port: u16,
    /// Admin API port
    pub admin_port: u16,
    /// Data directory
    pub data_dir: PathBuf,
    /// Initial cluster peers
    pub peers: Vec<String>,
    /// Bootstrap mode (first node in cluster)
    pub bootstrap: bool,
    /// Additional configuration options
    pub extra_config: HashMap<String, String>,
}

impl NodeConfig {
    /// Create a new node configuration
    pub fn new(id: NodeId, base_port: u16, data_dir: PathBuf) -> Self {
        Self {
            id,
            s3_port: base_port,
            grpc_port: base_port + 1000,
            admin_port: base_port + 2000,
            data_dir,
            peers: Vec::new(),
            bootstrap: false,
            extra_config: HashMap::new(),
        }
    }

    /// Mark this node as bootstrap node
    pub fn with_bootstrap(mut self) -> Self {
        self.bootstrap = true;
        self
    }

    /// Add a peer to the configuration
    pub fn with_peer(mut self, peer: String) -> Self {
        self.peers.push(peer);
        self
    }

    /// Add extra configuration option
    pub fn with_config(mut self, key: String, value: String) -> Self {
        self.extra_config.insert(key, value);
        self
    }

    /// Get the S3 endpoint URL
    pub fn s3_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.s3_port)
    }

    /// Get the gRPC endpoint URL
    pub fn grpc_endpoint(&self) -> String {
        format!("127.0.0.1:{}", self.grpc_port)
    }

    /// Get the admin endpoint URL
    pub fn admin_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.admin_port)
    }
}

/// Represents a running test node
pub struct TestNode {
    /// Node configuration
    pub config: NodeConfig,
    /// Process handle
    process: Option<Child>,
    /// Whether the node is currently partitioned
    partitioned: bool,
    /// Temporary directory (kept alive for the node lifetime)
    _temp_dir: Option<TempDir>,
}

impl TestNode {
    /// Create a new test node (not started)
    pub fn new(config: NodeConfig) -> Self {
        Self {
            config,
            process: None,
            partitioned: false,
            _temp_dir: None,
        }
    }

    /// Start the node process
    pub async fn start(&mut self, binary_path: &str) -> Result<()> {
        if self.process.is_some() {
            return Err(TestHarnessError::NodeStartFailure(
                "Node already running".to_string(),
            ));
        }

        info!("Starting node {} on S3 port {}", self.config.id, self.config.s3_port);

        // Create configuration file
        let config_path = self.config.data_dir.join("config.json");
        let config_json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&config_path, config_json)?;

        // Spawn the node process
        let child = Command::new(binary_path)
            .arg("--config")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| TestHarnessError::NodeStartFailure(e.to_string()))?;

        self.process = Some(child);

        // Wait for node to be ready
        self.wait_ready(Duration::from_secs(30)).await?;

        info!("Node {} started successfully", self.config.id);
        Ok(())
    }

    /// Stop the node process
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            info!("Stopping node {}", self.config.id);

            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                let pid = Pid::from_raw(child.id() as i32);
                if let Err(e) = kill(pid, Signal::SIGTERM) {
                    warn!("Failed to send SIGTERM to node {}: {}", self.config.id, e);
                }
            }

            // Wait a bit for graceful shutdown
            let wait_result = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::task::spawn_blocking(move || child.wait())
            ).await;

            match wait_result {
                Ok(Ok(Ok(status))) => {
                    debug!("Node {} stopped with status: {}", self.config.id, status);
                }
                Ok(Ok(Err(e))) => {
                    return Err(TestHarnessError::NodeStopFailure(e.to_string()));
                }
                Ok(Err(e)) => {
                    return Err(TestHarnessError::NodeStopFailure(e.to_string()));
                }
                Err(_) => {
                    warn!("Node {} did not stop gracefully within timeout", self.config.id);
                }
            }

            info!("Node {} stopped", self.config.id);
        }
        Ok(())
    }

    /// Kill the node immediately (simulate crash)
    pub async fn kill(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            info!("Killing node {} (simulated crash)", self.config.id);
            child.kill().map_err(|e| TestHarnessError::ProcessError(e.to_string()))?;
            child.wait().map_err(|e| TestHarnessError::ProcessError(e.to_string()))?;
            info!("Node {} killed", self.config.id);
        }
        Ok(())
    }

    /// Check if the node is running
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.process {
            child.try_wait().ok().flatten().is_none()
        } else {
            false
        }
    }

    /// Wait for the node to be ready
    async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        let start = std::time::Instant::now();
        let client = reqwest::Client::new();

        while start.elapsed() < timeout {
            match client.get(&format!("{}/health", self.config.admin_endpoint()))
                .timeout(Duration::from_secs(1))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    debug!("Node {} is ready", self.config.id);
                    return Ok(());
                }
                _ => {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Err(TestHarnessError::Timeout(format!(
            "Node {} did not become ready within {:?}",
            self.config.id, timeout
        )))
    }

    /// Get the node ID
    pub fn id(&self) -> NodeId {
        self.config.id
    }

    /// Get the S3 endpoint
    pub fn s3_endpoint(&self) -> String {
        self.config.s3_endpoint()
    }

    /// Get the admin endpoint
    pub fn admin_endpoint(&self) -> String {
        self.config.admin_endpoint()
    }

    /// Mark node as partitioned
    pub fn set_partitioned(&mut self, partitioned: bool) {
        self.partitioned = partitioned;
    }

    /// Check if node is partitioned
    pub fn is_partitioned(&self) -> bool {
        self.partitioned
    }
}

impl Drop for TestNode {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Manages a multi-node test cluster
pub struct TestCluster {
    /// Test nodes in the cluster
    nodes: Vec<TestNode>,
    /// Temporary directories for nodes
    temp_dirs: Vec<TempDir>,
    /// Base port for port allocation
    base_port: u16,
    /// Path to the anode binary
    binary_path: String,
}

impl TestCluster {
    /// Create a new test cluster with the specified number of nodes
    pub async fn new(num_nodes: usize) -> Result<Self> {
        Self::with_binary("target/debug/anode", num_nodes).await
    }

    /// Create a new test cluster with a specific binary path
    pub async fn with_binary(binary_path: &str, num_nodes: usize) -> Result<Self> {
        if num_nodes == 0 {
            return Err(TestHarnessError::InvalidConfiguration(
                "Cluster must have at least one node".to_string(),
            ));
        }

        let base_port = Self::find_available_port()?;
        let mut temp_dirs = Vec::new();
        let mut nodes = Vec::new();

        // Create node configurations
        for i in 0..num_nodes {
            let temp_dir = TempDir::new()?;
            let data_dir = temp_dir.path().to_path_buf();

            let node_id = i as NodeId;
            let node_port = base_port + (i as u16 * 100);
            let mut config = NodeConfig::new(node_id, node_port, data_dir);

            // First node is bootstrap node
            if i == 0 {
                config = config.with_bootstrap();
            } else {
                // Other nodes peer with the first node
                let bootstrap_peer = format!("127.0.0.1:{}", base_port + 1000);
                config = config.with_peer(bootstrap_peer);
            }

            nodes.push(TestNode::new(config));
            temp_dirs.push(temp_dir);
        }

        Ok(Self {
            nodes,
            temp_dirs,
            base_port,
            binary_path: binary_path.to_string(),
        })
    }

    /// Start all nodes in the cluster
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting cluster with {} nodes", self.nodes.len());

        // Start bootstrap node first
        if let Some(node) = self.nodes.first_mut() {
            node.start(&self.binary_path).await?;
            // Give bootstrap node time to initialize
            sleep(Duration::from_secs(2)).await;
        }

        // Start remaining nodes
        for node in self.nodes.iter_mut().skip(1) {
            node.start(&self.binary_path).await?;
            // Stagger node starts
            sleep(Duration::from_millis(500)).await;
        }

        info!("All nodes started");
        Ok(())
    }

    /// Stop all nodes in the cluster
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping cluster");

        for node in &mut self.nodes {
            if let Err(e) = node.stop().await {
                warn!("Failed to stop node {}: {}", node.id(), e);
            }
        }

        info!("Cluster stopped");
        Ok(())
    }

    /// Get a reference to a specific node
    pub fn get_node(&self, index: usize) -> Result<&TestNode> {
        self.nodes.get(index).ok_or_else(|| {
            TestHarnessError::InvalidConfiguration(format!("Node index {} out of bounds", index))
        })
    }

    /// Get a mutable reference to a specific node
    pub fn get_node_mut(&mut self, index: usize) -> Result<&mut TestNode> {
        self.nodes.get_mut(index).ok_or_else(|| {
            TestHarnessError::InvalidConfiguration(format!("Node index {} out of bounds", index))
        })
    }

    /// Get the S3 endpoint for the first node (for client connections)
    pub fn get_s3_endpoint(&self) -> String {
        self.nodes.first()
            .map(|n| n.s3_endpoint())
            .unwrap_or_else(|| "http://127.0.0.1:9000".to_string())
    }

    /// Wait for a leader to be elected
    pub async fn wait_for_leader(&self, timeout: Duration) -> Result<NodeId> {
        let start = std::time::Instant::now();
        let client = reqwest::Client::new();

        while start.elapsed() < timeout {
            for node in &self.nodes {
                if !node.is_partitioned() {
                    match client.get(&format!("{}/cluster/leader", node.admin_endpoint()))
                        .timeout(Duration::from_secs(1))
                        .send()
                        .await
                    {
                        Ok(response) if response.status().is_success() => {
                            if let Ok(leader_id) = response.json::<NodeId>().await {
                                info!("Leader elected: node {}", leader_id);
                                return Ok(leader_id);
                            }
                        }
                        _ => {}
                    }
                }
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(TestHarnessError::Timeout(format!(
            "No leader elected within {:?}",
            timeout
        )))
    }

    /// Kill a specific node (simulate crash)
    pub async fn kill_node(&mut self, index: usize) -> Result<()> {
        let node = self.get_node_mut(index)?;
        node.kill().await
    }

    /// Restart a specific node
    pub async fn restart_node(&mut self, index: usize) -> Result<()> {
        info!("Restarting node {}", index);
        let binary_path = self.binary_path.clone();
        let node = self.get_node_mut(index)?;
        node.stop().await?;
        sleep(Duration::from_secs(1)).await;
        node.start(&binary_path).await?;
        Ok(())
    }

    /// Partition a node from the network (simulate network partition)
    pub async fn partition_node(&mut self, index: usize) -> Result<()> {
        info!("Partitioning node {}", index);
        let node = self.get_node_mut(index)?;
        node.set_partitioned(true);

        // In a real implementation, this would use iptables or similar
        // For now, we just mark it as partitioned
        warn!("Network partitioning not fully implemented - node marked as partitioned");

        Ok(())
    }

    /// Heal a partitioned node
    pub async fn heal_partition(&mut self, index: usize) -> Result<()> {
        info!("Healing partition for node {}", index);
        let node = self.get_node_mut(index)?;
        node.set_partitioned(false);
        Ok(())
    }

    /// Get the number of nodes in the cluster
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.iter().map(|n| n.id()).collect()
    }

    /// Find an available port for the cluster
    fn find_available_port() -> Result<u16> {
        // Try to bind to a random port to find an available one
        use std::net::TcpListener;

        for base_port in (10000..60000).step_by(1000) {
            let mut all_available = true;

            // Check if we can get 100 consecutive ports (for multiple nodes)
            for offset in 0..100 {
                let port = base_port + offset;
                if TcpListener::bind(("127.0.0.1", port)).is_err() {
                    all_available = false;
                    break;
                }
            }

            if all_available {
                return Ok(base_port);
            }
        }

        Err(TestHarnessError::NetworkError(
            "Could not find available port range".to_string(),
        ))
    }
}

impl Drop for TestCluster {
    fn drop(&mut self) {
        // Ensure all nodes are stopped
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.block_on(async {
                let _ = self.stop().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config() {
        let config = NodeConfig::new(1, 9000, PathBuf::from("/tmp/test"));
        assert_eq!(config.id, 1);
        assert_eq!(config.s3_port, 9000);
        assert_eq!(config.grpc_port, 10000);
        assert_eq!(config.admin_port, 11000);
    }

    #[test]
    fn test_node_config_with_bootstrap() {
        let config = NodeConfig::new(1, 9000, PathBuf::from("/tmp/test"))
            .with_bootstrap();
        assert!(config.bootstrap);
    }

    #[test]
    fn test_find_available_port() {
        let port = TestCluster::find_available_port();
        assert!(port.is_ok());
    }
}
