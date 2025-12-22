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
//! use anode_storage::{StorageConfig, StorageEngine};
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
//! let storage_config = StorageConfig::new(config.data_dir.clone());
//! let storage_engine = Arc::new(StorageEngine::open(storage_config).await?);
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
pub use config::{RaftConfig, TlsConfig};
pub use error::{RaftError, Result};
pub use network::{RaftNetworkConnection, RaftNetworkManager};
pub use state_machine::{SnapshotBuilder, SnapshotData, StateMachine};
pub use storage::LogStore;
pub use types::{
    AnodeRaft, BucketConfig, Command, NodeId, Request, Response, ResponseData, TypeConfig,
};

use std::path::Path;
use std::sync::Arc;

/// Builder for creating a complete Raft node with all required components.
///
/// This builder simplifies the process of creating a Raft node by handling the
/// initialization of all necessary components (log store, state machine, network
/// manager) and wiring them together.
///
/// # Examples
///
/// ```no_run
/// use anode_raft::{RaftNodeBuilder, RaftConfig};
/// use anode_storage::StorageEngine;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RaftConfig::new(1, "my-cluster", PathBuf::from("/data/raft"));
///
/// let storage_config = anode_storage::StorageConfig {
///     data_dir: PathBuf::from("/data/blobs"),
///     metadata_dir: PathBuf::from("/data/metadata"),
///     chunk_size: 4 * 1024 * 1024,
///     sync_writes: false,
/// };
/// let storage = Arc::new(StorageEngine::open(storage_config).await?);
///
/// let raft = RaftNodeBuilder::new(config)
///     .storage_engine(storage)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct RaftNodeBuilder {
    /// Raft configuration
    config: RaftConfig,
    /// Storage engine for object data (required)
    storage_engine: Option<Arc<anode_storage::StorageEngine>>,
}

impl RaftNodeBuilder {
    /// Create a new builder with the specified configuration.
    ///
    /// The storage engine must be set using [`storage_engine`](Self::storage_engine)
    /// before calling [`build`](Self::build).
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::{RaftNodeBuilder, RaftConfig};
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data/raft"));
    /// let builder = RaftNodeBuilder::new(config);
    /// ```
    pub fn new(config: RaftConfig) -> Self {
        Self {
            config,
            storage_engine: None,
        }
    }

    /// Set the storage engine for object data (required).
    ///
    /// The storage engine handles the actual storage of object data and metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anode_raft::{RaftNodeBuilder, RaftConfig};
    /// use anode_storage::StorageEngine;
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data/raft"));
    ///
    /// let storage_config = anode_storage::StorageConfig {
    ///     data_dir: PathBuf::from("/data/blobs"),
    ///     metadata_dir: PathBuf::from("/data/metadata"),
    ///     chunk_size: 4 * 1024 * 1024,
    ///     sync_writes: false,
    /// };
    /// let storage = Arc::new(StorageEngine::open(storage_config).await?);
    ///
    /// let builder = RaftNodeBuilder::new(config)
    ///     .storage_engine(storage);
    /// # Ok(())
    /// # }
    /// ```
    pub fn storage_engine(mut self, storage: Arc<anode_storage::StorageEngine>) -> Self {
        self.storage_engine = Some(storage);
        self
    }

    /// Build and initialize the complete Raft node.
    ///
    /// This creates all necessary components (log store, state machine, network manager)
    /// and assembles them into a working Raft instance. The log and snapshot directories
    /// will be created if they don't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage engine was not set
    /// - Directories cannot be created
    /// - The log store cannot be opened
    /// - Raft initialization fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anode_raft::{RaftNodeBuilder, RaftConfig};
    /// use anode_storage::StorageEngine;
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data/raft"));
    ///
    /// let storage_config = anode_storage::StorageConfig {
    ///     data_dir: PathBuf::from("/data/blobs"),
    ///     metadata_dir: PathBuf::from("/data/metadata"),
    ///     chunk_size: 4 * 1024 * 1024,
    ///     sync_writes: false,
    /// };
    /// let storage = Arc::new(StorageEngine::open(storage_config).await?);
    ///
    /// let raft = RaftNodeBuilder::new(config)
    ///     .storage_engine(storage)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn build(self) -> Result<AnodeRaft> {
        let storage_engine = self
            .storage_engine
            .ok_or_else(|| RaftError::InvalidConfig("Storage engine not set".to_string()))?;

        // Ensure directories exist
        std::fs::create_dir_all(&self.config.log_dir())?;
        std::fs::create_dir_all(&self.config.snapshot_dir())?;

        // Create log store
        let log_store = LogStore::open(&self.config.log_dir().join("raft.db"))?;

        // Create state machine
        let state_machine = StateMachine::new(storage_engine, self.config.snapshot_dir());

        // Create network manager
        let network = RaftNetworkManager::new();

        let raft_config = Arc::new(self.config.to_openraft_config());

        // In storage-v2, Raft::new takes log_store and state_machine separately
        let raft = openraft::Raft::new(
            self.config.node_id,
            raft_config,
            network,
            log_store,
            state_machine,
        )
        .await
        .map_err(|e| RaftError::Fatal(format!("Failed to create Raft: {:?}", e)))?;

        Ok(raft)
    }
}

/// Convenience function to create a Raft node with default settings.
///
/// This is a shorthand for creating a [`RaftConfig`] with default parameters,
/// validating it, and building the Raft node. For more control over configuration,
/// use [`RaftNodeBuilder`] directly.
///
/// # Arguments
///
/// * `node_id` - Unique identifier for this node (must be non-zero)
/// * `cluster_name` - Name of the cluster this node belongs to
/// * `data_dir` - Root directory for Raft data (logs and snapshots)
/// * `storage_engine` - Storage engine for object data
///
/// # Errors
///
/// Returns an error if:
/// - The configuration is invalid
/// - Directories cannot be created
/// - Raft initialization fails
///
/// # Examples
///
/// ```no_run
/// use anode_raft::create_raft_node;
/// use anode_storage::StorageEngine;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let storage_config = anode_storage::StorageConfig {
///     data_dir: PathBuf::from("/data/blobs"),
///     metadata_dir: PathBuf::from("/data/metadata"),
///     chunk_size: 4 * 1024 * 1024,
///     sync_writes: false,
/// };
/// let storage = Arc::new(StorageEngine::open(storage_config).await?);
///
/// let raft = create_raft_node(
///     1,
///     "my-cluster",
///     PathBuf::from("/data/raft"),
///     storage
/// ).await?;
/// # Ok(())
/// # }
/// ```
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

    #[tokio::test]
    async fn test_builder_with_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = anode_storage::StorageConfig {
            data_dir: temp_dir.path().join("blobs"),
            metadata_dir: temp_dir.path().join("metadata"),
            chunk_size: anode_storage::chunks::CHUNK_SIZE,
            sync_writes: false,
        };
        let storage = Arc::new(
            anode_storage::StorageEngine::open(storage_config)
                .await
                .unwrap(),
        );

        let config = RaftConfig::default();
        let builder = RaftNodeBuilder::new(config).storage_engine(storage.clone());
        assert!(builder.storage_engine.is_some());
    }

    #[tokio::test]
    async fn test_create_raft_node() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("raft");

        let storage_config = anode_storage::StorageConfig {
            data_dir: temp_dir.path().join("blobs"),
            metadata_dir: temp_dir.path().join("metadata"),
            chunk_size: anode_storage::chunks::CHUNK_SIZE,
            sync_writes: false,
        };
        let storage = Arc::new(
            anode_storage::StorageEngine::open(storage_config)
                .await
                .unwrap(),
        );

        let result = create_raft_node(1, "test-cluster", data_dir, storage).await;
        assert!(result.is_ok());
    }
}
