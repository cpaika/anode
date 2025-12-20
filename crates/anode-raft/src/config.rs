use crate::types::NodeId;
use openraft::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the Raft node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// Unique identifier for this node
    pub node_id: NodeId,

    /// Name of the cluster this node belongs to
    pub cluster_name: String,

    /// Directory for storing Raft logs and snapshots
    pub data_dir: PathBuf,

    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,

    /// Minimum election timeout in milliseconds
    pub election_timeout_min_ms: u64,

    /// Maximum election timeout in milliseconds
    pub election_timeout_max_ms: u64,

    /// Number of logs after which to trigger snapshot
    pub snapshot_threshold: u64,

    /// Maximum number of entries in a single append entries request
    pub max_payload_entries: u64,

    /// Maximum size of snapshot chunk for transfer (in bytes)
    pub snapshot_chunk_size: u64,

    /// Enable or disable snapshot compression
    pub snapshot_compression: bool,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            cluster_name: "anode-cluster".to_string(),
            data_dir: PathBuf::from("/tmp/anode-raft"),
            heartbeat_interval_ms: 50,
            election_timeout_min_ms: 150,
            election_timeout_max_ms: 300,
            snapshot_threshold: 10000,
            max_payload_entries: 100,
            snapshot_chunk_size: 4 * 1024 * 1024, // 4MB
            snapshot_compression: true,
        }
    }
}

impl RaftConfig {
    /// Create a new RaftConfig with the given node ID
    pub fn new(node_id: NodeId, cluster_name: impl Into<String>, data_dir: PathBuf) -> Self {
        Self {
            node_id,
            cluster_name: cluster_name.into(),
            data_dir,
            ..Default::default()
        }
    }

    /// Convert to OpenRaft's Config type
    pub fn to_openraft_config(&self) -> Config {
        Config {
            heartbeat_interval: self.heartbeat_interval_ms,
            election_timeout_min: self.election_timeout_min_ms,
            election_timeout_max: self.election_timeout_max_ms,
            max_payload_entries: self.max_payload_entries,
            snapshot_policy: openraft::SnapshotPolicy::LogsSinceLast(self.snapshot_threshold),
            ..Default::default()
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.node_id == 0 {
            return Err("node_id must be non-zero".to_string());
        }

        if self.cluster_name.is_empty() {
            return Err("cluster_name cannot be empty".to_string());
        }

        if self.election_timeout_min_ms >= self.election_timeout_max_ms {
            return Err(
                "election_timeout_min_ms must be less than election_timeout_max_ms".to_string(),
            );
        }

        if self.heartbeat_interval_ms >= self.election_timeout_min_ms {
            return Err(
                "heartbeat_interval_ms must be less than election_timeout_min_ms".to_string(),
            );
        }

        if self.snapshot_threshold == 0 {
            return Err("snapshot_threshold must be greater than 0".to_string());
        }

        if self.max_payload_entries == 0 {
            return Err("max_payload_entries must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Get the log directory path
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Get the snapshot directory path
    pub fn snapshot_dir(&self) -> PathBuf {
        self.data_dir.join("snapshots")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RaftConfig::default();
        assert_eq!(config.node_id, 1);
        assert_eq!(config.cluster_name, "anode-cluster");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = RaftConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid node_id
        config.node_id = 0;
        assert!(config.validate().is_err());
        config.node_id = 1;

        // Invalid cluster_name
        config.cluster_name = String::new();
        assert!(config.validate().is_err());
        config.cluster_name = "test".to_string();

        // Invalid election timeout
        config.election_timeout_min_ms = 300;
        config.election_timeout_max_ms = 150;
        assert!(config.validate().is_err());
        config.election_timeout_min_ms = 150;
        config.election_timeout_max_ms = 300;

        // Invalid heartbeat interval
        config.heartbeat_interval_ms = 200;
        assert!(config.validate().is_err());
        config.heartbeat_interval_ms = 50;

        // Invalid snapshot threshold
        config.snapshot_threshold = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_to_openraft_config() {
        let config = RaftConfig::default();
        let openraft_config = config.to_openraft_config();

        assert_eq!(openraft_config.heartbeat_interval, config.heartbeat_interval_ms);
        assert_eq!(openraft_config.election_timeout_min, config.election_timeout_min_ms);
        assert_eq!(openraft_config.election_timeout_max, config.election_timeout_max_ms);
    }

    #[test]
    fn test_paths() {
        let config = RaftConfig::new(1, "test", PathBuf::from("/tmp/test"));
        assert_eq!(config.log_dir(), PathBuf::from("/tmp/test/logs"));
        assert_eq!(config.snapshot_dir(), PathBuf::from("/tmp/test/snapshots"));
    }
}
