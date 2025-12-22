use crate::types::NodeId;
use openraft::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TLS configuration for Raft inter-node communication.
///
/// This enables mutual TLS (mTLS) authentication between Raft nodes,
/// providing encryption and authentication for all Raft RPC traffic.
///
/// # Security
///
/// When `require_client_cert` is enabled, all nodes must present valid
/// certificates signed by the CA specified in `ca_file`. This prevents
/// unauthorized nodes from joining the cluster.
///
/// # Examples
///
/// ```
/// use anode_raft::TlsConfig;
/// use std::path::PathBuf;
///
/// // Configure mTLS
/// let tls_config = TlsConfig {
///     enabled: true,
///     cert_file: PathBuf::from("/etc/anode/tls/node.crt"),
///     key_file: PathBuf::from("/etc/anode/tls/node.key"),
///     ca_file: Some(PathBuf::from("/etc/anode/tls/ca.crt")),
///     require_client_cert: true,
///     verify_certificates: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS for Raft communication.
    ///
    /// When false, all other TLS settings are ignored.
    pub enabled: bool,

    /// Path to the server certificate file (PEM format).
    ///
    /// This certificate is presented when other nodes connect to this node.
    pub cert_file: PathBuf,

    /// Path to the server private key file (PEM format).
    pub key_file: PathBuf,

    /// Path to the CA certificate file for validating peer certificates.
    ///
    /// When set, incoming connections must present a certificate signed by this CA.
    /// This is the foundation of mutual TLS authentication.
    #[serde(default)]
    pub ca_file: Option<PathBuf>,

    /// Require client certificate authentication (mTLS).
    ///
    /// When true, all connecting peers must present valid client certificates.
    /// Only effective if `ca_file` is set.
    #[serde(default)]
    pub require_client_cert: bool,

    /// Enable TLS certificate verification.
    ///
    /// When false, certificates are not validated. This is useful for testing
    /// but should NEVER be used in production environments.
    #[serde(default = "default_verify_certs")]
    pub verify_certificates: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_file: PathBuf::new(),
            key_file: PathBuf::new(),
            ca_file: None,
            require_client_cert: false,
            verify_certificates: true,
        }
    }
}

fn default_verify_certs() -> bool {
    true
}

/// Configuration for a Raft node in the Anode cluster.
///
/// This configuration specifies all tunable parameters for the Raft consensus
/// protocol, including timing parameters, storage paths, and snapshot settings.
///
/// # Examples
///
/// ```
/// use anode_raft::RaftConfig;
/// use std::path::PathBuf;
///
/// // Create a config with default settings
/// let config = RaftConfig::new(1, "my-cluster", PathBuf::from("/data/raft"));
///
/// // Customize timing parameters
/// let config = config
///     .with_heartbeat_interval(100)
///     .with_election_timeout(300, 600);
///
/// // Validate before use
/// assert!(config.validate().is_ok());
/// ```
///
/// # Timing Parameters
///
/// The heartbeat interval must be significantly smaller than the election timeout
/// to prevent spurious elections. The recommended ratio is at least 3:1. The
/// election timeout should be randomized between min and max to prevent split votes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// Unique identifier for this node (must be non-zero).
    ///
    /// Each node in the cluster must have a unique ID. Node IDs are used
    /// for leader election, log replication, and cluster membership.
    pub node_id: NodeId,

    /// Name of the cluster this node belongs to.
    ///
    /// This is used for identification and logging purposes.
    pub cluster_name: String,

    /// Root directory for storing Raft logs and snapshots.
    ///
    /// The actual log and snapshot data will be stored in subdirectories:
    /// - `{data_dir}/logs/` - Raft log entries
    /// - `{data_dir}/snapshots/` - State machine snapshots
    pub data_dir: PathBuf,

    /// Interval between heartbeat messages in milliseconds.
    ///
    /// The leader sends heartbeats to followers at this interval to maintain
    /// leadership and prevent elections. Must be less than `election_timeout_min_ms`.
    ///
    /// Default: 50ms
    pub heartbeat_interval_ms: u64,

    /// Minimum election timeout in milliseconds.
    ///
    /// If a follower doesn't receive a heartbeat within this time, it will
    /// start an election. The actual timeout is randomized between min and max.
    ///
    /// Default: 150ms
    pub election_timeout_min_ms: u64,

    /// Maximum election timeout in milliseconds.
    ///
    /// The upper bound for the randomized election timeout. Must be greater
    /// than `election_timeout_min_ms`.
    ///
    /// Default: 300ms
    pub election_timeout_max_ms: u64,

    /// Number of log entries to accumulate before triggering a snapshot.
    ///
    /// Snapshots compact the log by saving the state machine's current state,
    /// allowing old log entries to be discarded. Smaller values use less disk
    /// space but create snapshots more frequently.
    ///
    /// Default: 10,000 entries
    pub snapshot_threshold: u64,

    /// Maximum number of log entries to send in a single append entries request.
    ///
    /// Limits the size of replication messages. Larger values improve throughput
    /// but increase memory usage and latency.
    ///
    /// Default: 100 entries
    pub max_payload_entries: u64,

    /// Maximum size of snapshot chunks for transfer (in bytes).
    ///
    /// Snapshots are streamed in chunks to avoid loading the entire snapshot
    /// into memory. Larger chunks improve transfer speed but use more memory.
    ///
    /// Default: 4 MB
    pub snapshot_chunk_size: u64,

    /// Enable or disable snapshot compression.
    ///
    /// When enabled, snapshots are compressed before storage and transmission,
    /// reducing disk usage and network bandwidth at the cost of CPU time.
    ///
    /// Default: true
    pub snapshot_compression: bool,

    /// TLS configuration for inter-node communication.
    ///
    /// When configured, all Raft RPC traffic will use TLS. If `require_client_cert`
    /// is enabled in the TLS config, mutual TLS authentication is enforced.
    ///
    /// Default: None (TLS disabled)
    #[serde(default)]
    pub tls: Option<TlsConfig>,
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
            tls: None,
        }
    }
}

impl RaftConfig {
    /// Create a new configuration with the specified node ID, cluster name, and data directory.
    ///
    /// All other parameters are set to their default values. Use the builder-style
    /// `with_*` methods to customize timing and snapshot parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "my-cluster", PathBuf::from("/data/raft"));
    /// assert_eq!(config.node_id, 1);
    /// assert_eq!(config.cluster_name, "my-cluster");
    /// ```
    pub fn new(node_id: NodeId, cluster_name: impl Into<String>, data_dir: PathBuf) -> Self {
        Self {
            node_id,
            cluster_name: cluster_name.into(),
            data_dir,
            ..Default::default()
        }
    }

    /// Set the election timeout range (builder pattern).
    ///
    /// The actual election timeout will be randomized between `min_ms` and `max_ms`
    /// to prevent split votes. The heartbeat interval must be less than `min_ms`.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data"))
    ///     .with_election_timeout(300, 600);
    ///
    /// assert_eq!(config.election_timeout_min_ms, 300);
    /// assert_eq!(config.election_timeout_max_ms, 600);
    /// ```
    pub fn with_election_timeout(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.election_timeout_min_ms = min_ms;
        self.election_timeout_max_ms = max_ms;
        self
    }

    /// Set the heartbeat interval (builder pattern).
    ///
    /// The leader sends heartbeats to followers at this interval. Must be
    /// significantly smaller than the election timeout (recommended ratio: 3:1).
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data"))
    ///     .with_heartbeat_interval(100);
    ///
    /// assert_eq!(config.heartbeat_interval_ms, 100);
    /// ```
    pub fn with_heartbeat_interval(mut self, interval_ms: u64) -> Self {
        self.heartbeat_interval_ms = interval_ms;
        self
    }

    /// Convert this configuration to OpenRaft's native `Config` type.
    ///
    /// This is used internally when creating the Raft instance. The conversion
    /// maps Anode's configuration parameters to their OpenRaft equivalents.
    ///
    /// # Returns
    ///
    /// An [`openraft::Config`] instance with parameters matching this configuration.
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

    /// Validate this configuration for correctness.
    ///
    /// Checks that all parameters are within valid ranges and satisfy
    /// required invariants (e.g., heartbeat < election timeout).
    ///
    /// # Errors
    ///
    /// Returns an error message if validation fails:
    /// - `node_id` must be non-zero
    /// - `cluster_name` must not be empty
    /// - `election_timeout_min_ms` must be less than `election_timeout_max_ms`
    /// - `heartbeat_interval_ms` must be less than `election_timeout_min_ms`
    /// - `snapshot_threshold` must be greater than 0
    /// - `max_payload_entries` must be greater than 0
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data"));
    /// assert!(config.validate().is_ok());
    ///
    /// let mut bad_config = config.clone();
    /// bad_config.node_id = 0;
    /// assert!(bad_config.validate().is_err());
    /// ```
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

    /// Get the directory path where Raft logs are stored.
    ///
    /// Returns `{data_dir}/logs/`.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data/raft"));
    /// assert_eq!(config.log_dir(), PathBuf::from("/data/raft/logs"));
    /// ```
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Get the directory path where snapshots are stored.
    ///
    /// Returns `{data_dir}/snapshots/`.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::RaftConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = RaftConfig::new(1, "cluster", PathBuf::from("/data/raft"));
    /// assert_eq!(config.snapshot_dir(), PathBuf::from("/data/raft/snapshots"));
    /// ```
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

        assert_eq!(
            openraft_config.heartbeat_interval,
            config.heartbeat_interval_ms
        );
        assert_eq!(
            openraft_config.election_timeout_min,
            config.election_timeout_min_ms
        );
        assert_eq!(
            openraft_config.election_timeout_max,
            config.election_timeout_max_ms
        );
    }

    #[test]
    fn test_paths() {
        let config = RaftConfig::new(1, "test", PathBuf::from("/tmp/test"));
        assert_eq!(config.log_dir(), PathBuf::from("/tmp/test/logs"));
        assert_eq!(config.snapshot_dir(), PathBuf::from("/tmp/test/snapshots"));
    }
}
