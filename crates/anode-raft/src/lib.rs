//! Anode Raft Integration Layer
//!
//! This crate provides a complete Raft consensus implementation for Anode using OpenRaft.
//! It includes:
//! - Type-safe Raft configuration with OpenRaft integration
//! - Persistent log storage using redb
//! - State machine implementation for object storage operations
//! - Network layer with gRPC support (placeholder - requires proto integration)
//!
//! # Example
//!
//! ```no_run
//! use anode_raft::{RaftConfig, LogStore, StateMachine, RaftNetworkManager};
//! use anode_storage::StorageEngine;
//! use std::sync::Arc;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create configuration
//! let config = RaftConfig::new(1, "my-cluster", PathBuf::from("/tmp/raft"));
//! config.validate()?;
//!
//! // Create storage components
//! let log_store = LogStore::open(&config.log_dir().join("raft.db"))?;
//! let storage_engine = Arc::new(StorageEngine::new(config.data_dir.clone())?);
//! let state_machine = StateMachine::new(storage_engine, config.snapshot_dir());
//!
//! // Create network manager
//! let network = RaftNetworkManager::new();
//!
//! // Build Raft node (requires combining log storage and state machine)
//! // let raft = openraft::Raft::new(...);
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod network;
pub mod state_machine;
pub mod storage;
pub mod types;

// Re-export main types
pub use config::RaftConfig;
pub use error::{RaftError, Result};
pub use network::{RaftNetworkConnection, RaftNetworkManager};
pub use state_machine::{SnapshotBuilder, SnapshotData, StateMachine};
pub use storage::LogStore;
pub use types::{AnodeRaft, BucketConfig, Command, NodeId, Request, Response, ResponseData, TypeConfig};

use openraft::storage::Adaptor;
use std::path::Path;
use std::sync::Arc;

/// Combined Raft storage that includes both log storage and state machine
pub type RaftStorage = Adaptor<TypeConfig, LogStore, StateMachine>;

/// Builder for creating a complete Raft node with all components
pub struct RaftNodeBuilder {
    config: RaftConfig,
    storage_engine: Option<Arc<anode_storage::StorageEngine>>,
}

impl RaftNodeBuilder {
    /// Create a new builder with the given configuration
    pub fn new(config: RaftConfig) -> Self {
        Self {
            config,
            storage_engine: None,
        }
    }

    /// Set the storage engine
    pub fn storage_engine(mut self, storage: Arc<anode_storage::StorageEngine>) -> Self {
        self.storage_engine = Some(storage);
        self
    }

    /// Build the Raft storage components
    pub fn build_storage(self) -> Result<RaftStorage> {
        let storage_engine = self.storage_engine.ok_or_else(|| {
            RaftError::InvalidConfig("Storage engine not set".to_string())
        })?;

        // Ensure directories exist
        std::fs::create_dir_all(&self.config.log_dir())?;
        std::fs::create_dir_all(&self.config.snapshot_dir())?;

        // Create log store
        let log_store = LogStore::open(&self.config.log_dir().join("raft.db"))?;

        // Create state machine
        let state_machine = StateMachine::new(storage_engine, self.config.snapshot_dir());

        // Combine into Raft storage
        Ok(Adaptor::new(log_store, state_machine))
    }

    /// Build a complete Raft node
    pub async fn build(self) -> Result<AnodeRaft> {
        let config = self.config.clone();
        let storage = self.build_storage()?;
        let network = RaftNetworkManager::new();

        let raft_config = config.to_openraft_config();
        let raft = openraft::Raft::new(config.node_id, raft_config, network, storage)
            .await
            .map_err(|e| RaftError::Fatal(format!("Failed to create Raft: {:?}", e)))?;

        Ok(raft)
    }
}

/// Convenience function to create a Raft node with default settings
pub async fn create_raft_node(
    node_id: NodeId,
    cluster_name: impl Into<String>,
    data_dir: impl AsRef<Path>,
    storage_engine: Arc<anode_storage::StorageEngine>,
) -> Result<AnodeRaft> {
    let config = RaftConfig::new(node_id, cluster_name, data_dir.as_ref().to_path_buf());
    config.validate().map_err(|e| RaftError::InvalidConfig(e))?;

    RaftNodeBuilder::new(config)
        .storage_engine(storage_engine)
        .build()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_builder_creation() {
        let config = RaftConfig::default();
        let builder = RaftNodeBuilder::new(config);
        assert!(builder.storage_engine.is_none());
    }

    #[test]
    fn test_builder_with_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(
            anode_storage::StorageEngine::new(temp_dir.path().to_path_buf()).unwrap(),
        );

        let config = RaftConfig::default();
        let builder = RaftNodeBuilder::new(config).storage_engine(storage.clone());
        assert!(builder.storage_engine.is_some());
    }

    #[tokio::test]
    async fn test_build_storage() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("raft");

        let storage = Arc::new(
            anode_storage::StorageEngine::new(temp_dir.path().to_path_buf()).unwrap(),
        );

        let config = RaftConfig::new(1, "test", data_dir);
        let raft_storage = RaftNodeBuilder::new(config)
            .storage_engine(storage)
            .build_storage()
            .unwrap();

        // Verify storage was created
        // The Adaptor type doesn't expose much publicly, but we can verify it was created
    }

    #[tokio::test]
    async fn test_create_raft_node() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("raft");

        let storage = Arc::new(
            anode_storage::StorageEngine::new(temp_dir.path().to_path_buf()).unwrap(),
        );

        let result = create_raft_node(1, "test-cluster", data_dir, storage).await;
        assert!(result.is_ok());
    }
}
